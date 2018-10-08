package com.padre.test;

import java.util.ArrayList;
import java.util.List;

public class ComplexJavaClass {
    private String test1;

    private boolean testMethod1(String testString) {
      return true;
    }

    private boolean testMethod2() {
      return true;
    }

    protected List<String> getAList() {
      List<String> result = new ArrayList<String>();
      return result;
    }

    public void testing()
        throws Exception
    {
        try
        {

            if ( testMethod1( test1 ) && testMethod2() )
            {
                System.out.println( "Some testing!!!" );
            }

            List aList = getAList();

            // if these are NOT set, just use the defaults, which are '${*}' and '@'.
            if ( aList != null && !aList.isEmpty() )
            {
                System.out.println(aList);
            }
        }
        catch ( Exception e )
        {
            throw new Exception( e.getMessage(), e );
        }
    }
}
